for (profile in listOf("debug", "release")) {
    val variant = profile.replaceFirstChar { it.uppercase() }
    val output = layout.buildDirectory.dir("networkSecurity/$profile")
    val policy = layout.projectDirectory.file("network-security-config.xml")
    val verifier = providers.provider {
        configurations.getByName("${profile}RuntimeClasspath").incoming.artifactView {
            componentFilter { id ->
                id is ModuleComponentIdentifier && id.group == "org.rustls" &&
                    id.module == "rustls-platform-verifier"
            }
        }.files
    }
    val merge = tasks.register("merge${variant}NetworkSecurity") {
        inputs.file(policy)
        inputs.files(verifier)
        outputs.dir(output)
        doLast {
            val factory = DocumentBuilderFactory.newInstance().apply {
                setFeature("http://apache.org/xml/features/disallow-doctype-decl", true)
                setFeature("http://xml.org/sax/features/external-general-entities", false)
                setFeature("http://xml.org/sax/features/external-parameter-entities", false)
            }
            val parser = factory.newDocumentBuilder()
            val document = ZipFile(verifier.get().singleFile).use { zip ->
                val entry = requireNotNull(zip.getEntry("res/xml/network_security_config.xml")) {
                    "The TLS verifier must supply its Network Security Config."
                }
                zip.getInputStream(entry).use { parser.parse(it) }
            }
            val base = (document.getElementsByTagName("base-config").item(0) as? Element)
                ?: document.createElement("base-config").also { document.documentElement.appendChild(it) }
            base.setAttribute("cleartextTrafficPermitted", "false")
            val custom = parser.parse(policy.asFile)
            val domains = custom.getElementsByTagName("domain")
            val hosts = (0 until domains.length).map { domains.item(it).textContent }.toSet()
            val existing = document.getElementsByTagName("domain")
            for (i in existing.length - 1 downTo 0) {
                val domain = existing.item(i) as Element
                // An existing verifier exception already permits this host and may include subdomains.
                if (domain.textContent.trim() in hosts) {
                    val duplicate = (0 until domains.length).map { domains.item(it) }
                        .firstOrNull { it.textContent == domain.textContent.trim() }
                    duplicate?.parentNode?.removeChild(duplicate)
                }
            }
            val additions = custom.getElementsByTagName("domain-config").item(0)
            if (custom.getElementsByTagName("domain").length > 0) {
                document.documentElement.appendChild(document.importNode(additions, true))
            }
            val target = output.get().file("xml/network_security_config.xml").asFile
            target.parentFile.mkdirs()
            TransformerFactory.newInstance().newTransformer()
                .transform(DOMSource(document), StreamResult(target))
        }
    }
    android.sourceSets.getByName(profile).res.directories.add(output.get().asFile.absolutePath)
    tasks.matching { it.name == "pre${variant}Build" }.configureEach { dependsOn(merge) }
}
